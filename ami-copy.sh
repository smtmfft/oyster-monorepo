#!/bin/bash

# Params : <profile> <source_region> <destination_region1> <destination_region2> <destination_region3>...

set -e

# set AWS_PROFILE 
export AWS_PROFILE=$1

# set AWS_DEFAULT_REGION 
export AWS_DEFAULT_REGION=$2

# List of regions the ami's will be copied to
regions=("${@:3}")

# Fetching ImageID for arm64 architecture
ami_arm=$(aws ec2 describe-images --owners self --filters Name=name,Values=MarlinLauncherARM64 --no-paginate --query 'Images[0].ImageId')
ami_arm=$(echo $ami_arm| cut -d'"' -f 2)
echo "Source image id for ARM64 : $ami_arm"

# Fetching ImageID for x86_64 architecture
ami_amd=$(aws ec2 describe-images --owners self --filters Name=name,Values=MarlinLauncherx86_64 --no-paginate --query 'Images[0].ImageId')
ami_amd=$(echo $ami_amd| cut -d'"' -f 2)
echo "Source image id for x86_64 : $ami_amd"

# Copying both AMI's to each of the secified regions
for r in ${regions[@]}; do 
    echo "Copying for region : $r"
    aws ec2 copy-image --name MarlinLauncherARM64 --source-image-id $ami_arm --source-region $2 --region $r --copy-image-tags
    aws ec2 copy-image --name MarlinLauncherx86_64 --source-image-id $ami_amd --source-region $2 --region $r --copy-image-tags
done    
