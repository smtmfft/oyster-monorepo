#!/bin/bash

# Params : <profile> <source_region>

# set AWS_PROFILE 
export AWS_PROFILE=$1

# List of regions the ami's will be copied to
regions=("ap-southeast-1" "us-east-1" "us-east-2" "us-west-1" "us-west-2" "ca-central-1"
    \ "eu-north-1" "eu-west-3" "eu-west-2" "eu-west-1" "eu-central-1" "eu-south-1"
    \ "ap-south-1" "ap-northeast-1" "ap-northeast-2" "ap-southeast-1" "ap-southeast-2" "ap-east-1")

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