#!/bin/bash

# Params : <profile> <date> <region1> <region2> <region3>...

set -e

# set AWS_PROFILE
export AWS_PROFILE=$1

# List of regions where the amis will be made public
regions=("${@:3}")

# Copying both AMI's to each of the secified regions
for r in ${regions[@]}; do
    ami_amd=$(AWS_REGION=$r aws ec2 describe-images --owners self --filters "Name=name,Values=marlin/oyster/worker-amd64-$2" --no-paginate --query 'Images[0].ImageId' --output text)
    ami_arm=$(AWS_REGION=$r aws ec2 describe-images --owners self --filters "Name=name,Values=marlin/oyster/worker-arm64-$2" --no-paginate --query 'Images[0].ImageId' --output text)
    if [[ $ami_amd != null && "$ami_amd" != "None" ]]; then
        echo "Making public amd64 image in region $r: $ami_amd"
        AWS_REGION=$r aws ec2 modify-image-attribute --image-id $ami_amd --launch-permission "Add=[{Group=all}]"
    else
        echo "Found no amd64 image in $r"
    fi
    if [[ $ami_arm != null && "$ami_arm" != "None" ]]; then
        echo "Making public arm64 image in region $r: $ami_arm"
        AWS_REGION=$r aws ec2 modify-image-attribute --image-id $ami_arm --launch-permission "Add=[{Group=all}]"
    else
        echo "Found no arm64 image in $r"
    fi
done
